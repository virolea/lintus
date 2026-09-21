# frozen_string_literal: true

require "json"

module Lintus
  class Formatter
    # Machine-readable output for other tools to consume.
    class Json < Formatter
      def render(report)
        io.puts ::JSON.pretty_generate(
          summary: {
            files_inspected: report.checked.size,
            offenses: report.offenses.size,
            skipped: report.skipped.size,
            failures: report.failures.size
          },
          offenses: report.sorted_offenses.map(&:to_h),
          files: report.checked.sort_by(&:path).map { |checked| file_entry(checked) },
          skipped: report.skipped.map { |skipped| { path: skipped.path, reason: skipped.reason } },
          failures: report.failures.map { |failure| { path: failure.path, error: failure.error.message } }
        )
      end

      private

      # Every rule asked about the file and the probability it got, offense or not.
      def file_entry(checked)
        { path: checked.path, answers: checked.answers.transform_values { |noul| noul.round(3) } }
      end
    end
  end
end
