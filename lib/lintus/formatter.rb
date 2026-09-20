# frozen_string_literal: true

module Lintus
  # Base class: subclasses render a Report to an IO.
  class Formatter
    FORMATS = { "text" => "Text", "github" => "GitHub", "json" => "JSON" }.freeze

    def self.for(name)
      const_name = FORMATS[name.to_s] or raise Error,
                                               "unknown format #{name.inspect} (expected one of #{FORMATS.keys.join(", ")})"
      const_get(const_name)
    end

    def self.names = FORMATS.keys

    attr_reader :io

    def initialize(io = $stdout)
      @io = io
    end

    def render(report)
      raise NotImplementedError
    end

    private

    def summary(report)
      parts = ["#{pluralize(report.checked.size, "file")} inspected",
               "#{pluralize(report.offenses.size, "offense")} detected"]
      parts << "#{pluralize(report.skipped.size, "file")} skipped" if report.skipped.any?
      parts << "#{pluralize(report.failures.size, "file")} failed" if report.failures.any?
      parts.join(", ")
    end

    def pluralize(count, noun) = "#{count} #{noun}#{"s" unless count == 1}"
  end
end
