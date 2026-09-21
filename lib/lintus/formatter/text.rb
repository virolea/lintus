# frozen_string_literal: true

module Lintus
  class Formatter
    # Human-readable output, one line per offense.
    class Text < Formatter
      private

      def offense_line(offense)
        "#{offense.path}: [#{offense.rule.id}] #{offense.message} (noul #{offense.noul.round(2)})"
      end

      def skipped_line(skipped) = "#{skipped.path}: skipped, #{skipped.reason}"

      def failure_line(failure) = "#{failure.path}: failed, #{failure.error.message}"

      def before_summary(report)
        io.puts if report.offenses.any? || report.skipped.any? || report.failures.any?
      end
    end
  end
end
