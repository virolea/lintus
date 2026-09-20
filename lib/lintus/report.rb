# frozen_string_literal: true

module Lintus
  # Everything a run produced: what was checked, what was flagged, what was skipped, what broke.
  class Report
    Checked = Struct.new(:path, :rules, :model, keyword_init: true)
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

    def record_checked(path, rules:, model:)
      synchronize { checked << Checked.new(path: path, rules: rules, model: model) }
    end

    def record_offenses(offenses)
      synchronize { self.offenses.concat(offenses) }
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

    def failed?(fail_on: "error")
      return true if failures.any?

      case fail_on.to_s
      when "warning" then offenses.any?
      when "never" then false
      else errors.any?
      end
    end

    def exit_status(fail_on: "error")
      return 2 if failures.any?

      failed?(fail_on: fail_on) ? 1 : 0
    end

    private

    def synchronize(&) = @mutex.synchronize(&)
  end
end
