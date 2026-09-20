# frozen_string_literal: true

module Lintus
  # The parsed config file. Its directory is the root every path is relative to.
  class Config
    DEFAULT_MAX_FILE_SIZE = 100_000
    KEYS = %w[rules paths exclude max_file_size].freeze

    attr_reader :root, :rules, :paths, :exclude, :max_file_size

    class << self
      def load(path = nil, dir: Dir.pwd)
        path ||= locate(dir)
        unless path
          raise ConfigError,
                "No config file found: looked for #{CONFIG_FILENAMES.join(", ")} in #{dir} and its parents"
        end
        raise ConfigError, "Config file not found: #{path}" unless File.file?(path)

        path = File.expand_path(path)
        new(parse(path), root: File.dirname(path))
      end

      # Walks up from `dir` and returns the first config file found, like git does for .git.
      def locate(dir)
        Pathname(dir).expand_path.ascend do |ancestor|
          CONFIG_FILENAMES.each do |name|
            candidate = ancestor / name
            return candidate.to_s if candidate.file?
          end
        end
        nil
      end

      private

      def parse(path)
        YAML.safe_load_file(path, permitted_classes: [Symbol], aliases: true) || {}
      rescue Psych::SyntaxError => e
        raise ConfigError, "#{path} is not valid YAML: #{e.message}"
      end
    end

    def initialize(data, root: Dir.pwd)
      raise ConfigError, "config must be a map, got #{data.class}" unless data.is_a?(Hash)

      data = data.transform_keys(&:to_s)
      Schema.reject_unknown_keys!(data, KEYS, context: "config")

      @root = File.expand_path(root)
      @paths = Schema.string_list(data["paths"])
      @exclude = Schema.string_list(data["exclude"])
      @max_file_size = build_max_file_size(data.fetch("max_file_size", DEFAULT_MAX_FILE_SIZE))
      @rules = build_rules(data["rules"])
    end

    # The rules that apply to a repository-relative path, in config order.
    def rules_for(path) = rules.select { |rule| rule.applies_to?(path) }

    def rule(id) = rules.find { |rule| rule.id == id.to_s }

    private

    def build_max_file_size(value)
      raise ConfigError, "`max_file_size` must be a positive integer of bytes" unless value.is_a?(Integer) && value.positive?

      value
    end

    def build_rules(rules)
      raise ConfigError, "`rules` must be a map of rule id => attributes" unless rules.is_a?(Hash)
      raise ConfigError, "`rules` is empty: nothing to lint" if rules.empty?

      rules.map { |id, attrs| Rule.new(id, attrs, default_paths: paths, default_exclude: exclude) }
    end
  end
end
