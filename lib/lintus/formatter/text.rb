# frozen_string_literal: true

module Lintus
  class Formatter
    # Human-readable output, one line per offense.
    class Text < Formatter
      def render(report)
        report.sorted_offenses.each do |offense|
          io.puts "#{offense.path}: [#{offense.rule.id}] #{offense.message} (#{offense.severity}, noul #{offense.noul.round(2)})"
        end
        report.skipped.each { |skipped| io.puts "#{skipped.path}: skipped, #{skipped.reason}" }
        report.failures.each { |failure| io.puts "#{failure.path}: failed, #{failure.error.message}" }

        io.puts if report.offenses.any? || report.skipped.any? || report.failures.any?
        io.puts summary(report)
      end
    end
  end
end
