# frozen_string_literal: true

module Lintus
  # Renders a Report to an IO. Line-oriented subclasses override the three
  # `*_line` hooks; anything else overrides `render`.
  class Formatter
    NAMES = %w[text github json].freeze

    def self.for(name)
      raise Error, "unknown format #{name.inspect} (expected one of #{NAMES.join(", ")})" unless NAMES.include?(name.to_s)

      const_get(name.to_s.capitalize)
    end

    attr_reader :io

    def initialize(io = $stdout)
      @io = io
    end

    def render(report)
      report.sorted_offenses.each { |offense| io.puts offense_line(offense) }
      report.skipped.each { |skipped| io.puts skipped_line(skipped) }
      report.failures.each { |failure| io.puts failure_line(failure) }
      before_summary(report)
      io.puts summary(report)
    end

    private

    def offense_line(offense) = raise(NotImplementedError)
    def skipped_line(skipped) = raise(NotImplementedError)
    def failure_line(failure) = raise(NotImplementedError)

    def before_summary(report); end

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
