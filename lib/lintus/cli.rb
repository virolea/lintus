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

    Options = Struct.new(
      :config, :mode, :ref, :format, :jobs, :api_key, :list, :fail_on, :quiet,
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
      options = parse(argv.dup)
      return init if options == :init
      return 0 if options == :done

      config = Config.load(options.config, dir: @dir)
      files = find_files(config, options)
      runner = Runner.new(config, jobs: options.jobs)
      tasks = runner.plan(files)

      return list(tasks) if options.list

      configure_api_key!(options)
      report = runner.run(tasks.map(&:first))
      Formatter.for(options.format).new(stdout).render(report)
      report.exit_status(fail_on: options.fail_on)
    rescue Error, Jev::Error, OptionParser::ParseError => e
      stderr.puts "lintus: #{e.message}"
      2
    end

    private

    def parse(argv)
      options = default_options
      parser = build_parser(options)
      argv = parser.parse(argv)
      return :done if options.mode == :done

      if argv.first == "init"
        return :init if argv.size == 1

        raise OptionParser::InvalidArgument, "init takes no arguments"
      end

      if argv.any?
        if options.mode != :all
          raise OptionParser::InvalidArgument,
                "FILE arguments cannot be combined with --diff or --staged"
        end

        options.mode = :explicit
        options.ref = argv
      end
      options
    end

    def default_options
      Options.new(
        mode: :all, format: @env["GITHUB_ACTIONS"] == "true" ? "github" : "text", jobs: 4,
        api_key: @env["JEV_API_KEY"], list: false, fail_on: "error"
      )
    end

    def build_parser(options) # rubocop:disable Metrics/AbcSize, Metrics/MethodLength
      OptionParser.new do |parser| # rubocop:disable Metrics/BlockLength
        parser.banner = USAGE
        parser.separator ""
        parser.separator "Which files:"
        parser.on("-d", "--diff [REF]", "Only files changed since REF (default HEAD: uncommitted changes)") do |ref|
          options.mode = :diff
          options.ref = ref || "HEAD"
        end
        parser.on("-s", "--staged", "Only staged files, reading their staged content (for pre-commit hooks)") do
          options.mode = :staged
        end
        parser.separator ""
        parser.separator "How to run:"
        parser.on("-c", "--config PATH",
                  "Config file (default: nearest #{CONFIG_FILENAMES.first} upwards from the current directory)") do |path|
          options.config = path
        end
        parser.on("-f", "--format FORMAT", Formatter.names,
                  "Output format: #{Formatter.names.join(", ")} (default: text, or github under GitHub Actions)") do |format|
          options.format = format
        end
        parser.on("-j", "--jobs N", Integer, "Concurrent requests to the Jev API (default 4)") do |jobs|
          options.jobs = jobs
        end
        parser.on("--api-key KEY", "Jev API key (default: $JEV_API_KEY)") { |key| options.api_key = key }
        parser.on("--fail-on LEVEL", %w[error warning never],
                  "Exit non-zero on: error (default), warning, never") do |level|
          options.fail_on = level
        end
        parser.on("-l", "--list", "List the files and rules that would be checked, without calling the API") do
          options.list = true
        end
        parser.separator ""
        parser.on("-v", "--version", "Print the version") do
          stdout.puts "lintus #{VERSION}"
          options.mode = :done
        end
        parser.on("-h", "--help", "Print this help") do
          stdout.puts parser
          options.mode = :done
        end
      end
    end

    def find_files(config, options)
      finder = FileFinder.new(config.root)

      case options.mode
      when :diff then finder.changed_since(options.ref)
      when :staged then finder.staged
      when :explicit then finder.explicit(options.ref, from: @dir)
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
