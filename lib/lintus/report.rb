# frozen_string_literal: true

module Lintus
  # Everything a run produced: what was checked, what was flagged, what was skipped, what broke.
  class Report
    Checked = Struct.new(:path, :answers, keyword_init: true)
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

    # `answers` maps every rule asked about the file to the probability it got,
    # offense or not, so a run can be read for confidence and not just verdicts.
    def record_checked(path, offenses, answers = {})
      synchronize do
        checked << Checked.new(path: path, answers: answers)
        self.offenses.concat(offenses)
      end
    end

    def record_skipped(path, reason)
      synchronize { skipped << Skipped.new(path: path, reason: reason) }
    end

    def record_failure(path, error)
      synchronize { failures << Failure.new(path: path, error: error) }
    end

    def sorted_offenses = offenses.sort_by { |offense| [offense.path, offense.rule.id] }

    # 2 when a file could not be checked, 1 when there are offenses, else 0.
    def exit_status
      return 2 if failures.any?

      offenses.any? ? 1 : 0
    end

    private

    def synchronize(&) = @mutex.synchronize(&)
  end
end
