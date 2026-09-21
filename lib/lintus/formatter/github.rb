# frozen_string_literal: true

module Lintus
  class Formatter
    # GitHub Actions workflow commands: each offense becomes an annotation on
    # its file in the pull request, then a plain summary for the job log.
    class Github < Formatter
      private

      def offense_line(offense)
        command("error", offense.message, file: offense.path, title: "lintus: #{offense.rule.id}")
      end

      def skipped_line(skipped)
        command("notice", "Skipped: #{skipped.reason}", file: skipped.path, title: "lintus")
      end

      def failure_line(failure)
        command("error", failure.error.message, file: failure.path, title: "lintus: request failed")
      end

      def command(level, message, **properties)
        props = properties.map { |key, value| "#{key}=#{escape_property(value)}" }.join(",")
        "::#{level} #{props}::#{escape_data(message)}"
      end

      def escape_data(value) = value.to_s.gsub("%", "%25").gsub("\r", "%0D").gsub("\n", "%0A")

      def escape_property(value) = escape_data(value).gsub(":", "%3A").gsub(",", "%2C")
    end
  end
end
