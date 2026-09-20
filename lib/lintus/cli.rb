# frozen_string_literal: true

require "optparse"

module Lintus
  # The `lintus` command.
  class CLI
    USAGE = <<~USAGE
      Usage: lintus [options] [FILE...]
             lintus init

      With no FILE, lints every file in the tree. FILE may be a file or a directory.
    USAGE

    # `command` is what to do; `selection` is which files, with `ref` for --diff
    # and `files` for paths given on the command line.
    Options = Struct.new(
      :command, :selection, :ref, :files, :config, :format, :jobs, :api_key, :list, :fail_on,
      keyword_init: true
    )

    attr_reader :stdout, :stderr

    def initialize(stdout: $stdout, stderr: $stderr, env: ENV, dir: Dir.pwd)
      @stdout = stdout
      @stderr = stderr
      @env = env
      @dir = dir
    end

    def run(argv)
      options = parse(argv)

      case options.command
      when :exit then 0
      when :init then init
      else lint(options)
      end
    rescue Error, Jev::Error, OptionParser::ParseError => e
      stderr.puts "lintus: #{e.message}"
      2
    end

    private

    def lint(options)
      config = Config.load(options.config, dir: @dir)
      runner = Runner.new(config, jobs: options.jobs)
      tasks = runner.plan(find_files(config, options))
      return list(tasks) if options.list

      configure_api_key!(options)
      report = runner.run(tasks)
      Formatter.for(options.format).new(stdout).render(report)
      report.exit_status(fail_on: options.fail_on)
    end

    def parse(argv)
      options = default_options
      positional = build_parser(options).parse(argv)
      return options if options.command == :exit

      if positional.first == "init"
        raise OptionParser::InvalidArgument, "init takes no arguments" if positional.size > 1

        options.command = :init
      elsif positional.any?
        if options.selection != :all
          raise OptionParser::InvalidArgument,
                "FILE arguments cannot be combined with --diff or --staged"
        end

        options.selection = :explicit
        options.files = positional
      end
      options
    end

    def default_options
      Options.new(
        command: :lint, selection: :all, format: @env["GITHUB_ACTIONS"] == "true" ? "github" : "text",
        jobs: 4, api_key: @env["JEV_API_KEY"], list: false, fail_on: "error"
      )
    end

    def build_parser(options) # rubocop:disable Metrics/MethodLength
      OptionParser.new do |parser| # rubocop:disable Metrics/BlockLength
        parser.banner = USAGE
        parser.separator ""
        parser.separator "Which files:"
        parser.on("-d", "--diff [REF]", "Only files changed since REF (default HEAD: uncommitted changes)") do |ref|
          options.selection = :diff
          options.ref = ref || "HEAD"
        end
        parser.on("-s", "--staged", "Only staged files, reading their staged content (for pre-commit hooks)") do
          options.selection = :staged
        end
        parser.separator ""
        parser.separator "How to run:"
        parser.on("-c", "--config PATH",
                  "Config file (default: nearest #{CONFIG_FILENAMES.first} upwards from the current directory)") do |path|
          options.config = path
        end
        parser.on("-f", "--format FORMAT", Formatter::NAMES,
                  "Output format: #{Formatter::NAMES.join(", ")} (default: text, or github under GitHub Actions)") do |format|
          options.format = format
        end
        parser.on("-j", "--jobs N", Integer, "Concurrent requests to the Jev API (default 4)") do |jobs|
          options.jobs = jobs
        end
        parser.on("--api-key KEY", "Jev API key (default: $JEV_API_KEY)") { |key| options.api_key = key }
        parser.on("--fail-on LEVEL", %w[error warning never], "Exit non-zero on: error (default), warning, never") do |level|
          options.fail_on = level
        end
        parser.on("-l", "--list", "List the files and rules that would be checked, without calling the API") do
          options.list = true
        end
        parser.separator ""
        parser.on("-v", "--version", "Print the version") do
          stdout.puts "lintus #{VERSION}"
          options.command = :exit
        end
        parser.on("-h", "--help", "Print this help") do
          stdout.puts parser
          options.command = :exit
        end
      end
    end

    def find_files(config, options)
      finder = FileFinder.new(config.root)

      case options.selection
      when :diff then finder.changed_since(options.ref)
      when :staged then finder.staged
      when :explicit then finder.explicit(options.files, from: @dir)
      else finder.all
      end
    end

    def configure_api_key!(options)
      raise Error, "no API key: set JEV_API_KEY or pass --api-key" if options.api_key.nil? || options.api_key.empty?

      Jev.api_key = options.api_key
    end

    def list(tasks)
      tasks.each { |file, rules| stdout.puts "#{file.path}: #{rules.map(&:id).join(", ")}" }
      stdout.puts "#{tasks.size} file(s) would be checked"
      0
    end

    def init
      path = File.join(@dir, CONFIG_FILENAMES.first)
      raise Error, "#{path} already exists" if File.exist?(path)

      File.write(path, Template::CONFIG)
      stdout.puts "Wrote #{path}"
      0
    end
  end
end
