# frozen_string_literal: true

module Lintus
  # Everything a run produced: what was checked, what was flagged, what was skipped, what broke.
  class Report
    Skipped = Struct.new(:path, :reason, keyword_init: true)
    Failure = Struct.new(:path, :error, keyword_init: true)

    attr_reader :checked, :offenses, :skipped, :failures

    def initialize
      @checked = []
      @offenses = []
      @skipped = []
      @failures = []
      @mutex = Mutex.new
    end

    def record_checked(path, offenses)
      synchronize do
        checked << path
        self.offenses.concat(offenses)
      end
    end

    def record_skipped(path, reason)
      synchronize { skipped << Skipped.new(path: path, reason: reason) }
    end

    def record_failure(path, error)
      synchronize { failures << Failure.new(path: path, error: error) }
    end

    def errors = offenses.select(&:error?)
    def warnings = offenses.reject(&:error?)

    def sorted_offenses = offenses.sort_by { |offense| [offense.path, offense.rule.id] }

    # 2 when a file could not be checked, 1 when offenses reach `fail_on`, else 0.
    def exit_status(fail_on: "error")
      return 2 if failures.any?

      failing = case fail_on.to_s
                when "warning" then offenses
                when "never" then []
                else errors
                end
      failing.any? ? 1 : 0
    end

    private

    def synchronize(&) = @mutex.synchronize(&)
  end
end
