# frozen_string_literal: true

require "json"

module Lintus
  class Formatter
    # Machine-readable output for other tools to consume.
    class JSON < Formatter
      def render(report)
        io.puts ::JSON.pretty_generate(
          summary: {
            files_inspected: report.checked.size,
            offenses: report.offenses.size,
            errors: report.errors.size,
            warnings: report.warnings.size,
            skipped: report.skipped.size,
            failures: report.failures.size
          },
          offenses: report.sorted_offenses.map(&:to_h),
          skipped: report.skipped.map { |skipped| { path: skipped.path, reason: skipped.reason } },
          failures: report.failures.map { |failure| { path: failure.path, error: failure.error.message } }
        )
      end
    end
  end
end
