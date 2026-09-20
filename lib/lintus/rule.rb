# frozen_string_literal: true

module Lintus
  # One entry of the `rules:` map in the config file.
  #
  # A rule is a noul question. It is asked against every file whose path
  # matches its globs, and the file is an offense when the answer equals
  # `offense_when` (true by default, so questions are phrased to describe the
  # offense: "Does this file call sleep?").
  class Rule
    SEVERITIES = %w[error warning].freeze
    ID_FORMAT = /\A[a-z][a-z0-9_]*\z/

    attr_reader :id, :description, :question, :criteria, :threshold, :paths, :exclude, :severity, :offense_when

    def initialize(id, attrs, default_paths: [])
      @id = validate_id(id)
      attrs = normalize(attrs)

      @question = fetch_string(attrs, "question")
      @description = attrs.fetch("description", @question).to_s
      @criteria = build_criteria(attrs["criteria"])
      @threshold = build_threshold(attrs["threshold"])
      @paths = Array(attrs.fetch("paths", default_paths)).map(&:to_s)
      @exclude = Array(attrs["exclude"]).map(&:to_s)
      @severity = build_severity(attrs.fetch("severity", "error"))
      @offense_when = build_offense_when(attrs.fetch("offense_when", true))
    end

    def applies_to?(path)
      return false if Glob.match_any?(exclude, path)

      paths.empty? || Glob.match_any?(paths, path)
    end

    def add_to(query)
      options = { criteria: criteria, threshold: threshold }.compact
      query.ask(id, question, **options)
    end

    def offense?(answer) = answer.result == offense_when

    def error? = severity == "error"

    private

    def validate_id(id)
      id = id.to_s
      unless id.match?(ID_FORMAT)
        raise ConfigError,
              "invalid rule id #{id.inspect}: use snake_case (letters, digits, underscores)"
      end

      id
    end

    def normalize(attrs)
      raise ConfigError, "rule #{id}: expected a map of attributes, got #{attrs.inspect}" unless attrs.is_a?(Hash)

      attrs.transform_keys(&:to_s)
    end

    def fetch_string(attrs, key)
      value = attrs[key]
      raise ConfigError, "rule #{id}: `#{key}` is required" if value.nil? || value.to_s.strip.empty?

      value.to_s.strip
    end

    def build_criteria(criteria)
      return nil if criteria.nil?
      raise ConfigError, "rule #{id}: `criteria` must be a map with `true` and `false` keys" unless criteria.is_a?(Hash)

      criteria.to_h { |key, value| [key.to_s, value.to_s] }
    end

    def build_threshold(threshold)
      return nil if threshold.nil?

      valid = threshold.is_a?(Numeric) && threshold.between?(0, 1)
      raise ConfigError, "rule #{id}: `threshold` must be a number between 0 and 1" unless valid

      threshold.to_f
    end

    def build_severity(severity)
      severity = severity.to_s
      unless SEVERITIES.include?(severity)
        raise ConfigError,
              "rule #{id}: `severity` must be one of #{SEVERITIES.join(", ")}"
      end

      severity
    end

    def build_offense_when(value)
      raise ConfigError, "rule #{id}: `offense_when` must be true or false" unless [true, false].include?(value)

      value
    end
  end
end
