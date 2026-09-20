# frozen_string_literal: true

module Lintus
  # Sends each file to Jev once, with every rule that applies to it as a
  # separate noul question, and turns the answers into offenses.
  class Runner
    RETRYABLE = [Jev::RateLimitError, Jev::OverloadedError].freeze

    attr_reader :config, :jobs, :retries

    def initialize(config, jobs: 4, retries: 3, sleeper: Kernel.method(:sleep), progress: nil)
      @config = config
      @jobs = [jobs.to_i, 1].max
      @retries = retries
      @sleeper = sleeper
      @progress = progress
    end

    # Pairs every file with the rules that apply to it, dropping files no rule covers.
    def plan(files)
      files.filter_map do |file|
        rules = config.rules_for(file.path)
        [file, rules] unless rules.empty?
      end
    end

    def run(files)
      report = Report.new
      queue = Queue.new
      plan(files).each { |task| queue << task }
      queue.close

      workers = Array.new([jobs, queue.size].min) do
        Thread.new do
          while (task = queue.pop)
            check(*task, report)
          end
        end
      end
      workers.each(&:join)

      report
    end

    def check(file, rules, report)
      if (reason = skip_reason(file))
        report.record_skipped(file.path, reason)
        return
      end

      response = with_retries { perform(file, rules) }
      report.record_checked(file.path, rules: rules, model: response.model)
      report.record_offenses(offenses_for(file, rules, response.answers))
    rescue Jev::Error, Git::CommandError, SystemCallError => e
      report.record_failure(file.path, e)
    ensure
      @progress&.call(file)
    end

    private

    def skip_reason(file)
      return "binary file" if file.binary?
      return "larger than max_file_size (#{file.size} > #{config.max_file_size} bytes)" if file.size > config.max_file_size

      nil
    end

    def perform(file, rules)
      Jev::Query.new(state_for(file)).perform do |query|
        rules.each { |rule| rule.add_to(query) }
      end
    end

    def state_for(file) = "File: #{file.path}\n\n#{file.content}"

    def offenses_for(file, rules, answers)
      rules.filter_map do |rule|
        answer = answers[rule.id]
        Offense.new(path: file.path, rule: rule, noul: answer.noul) if rule.offense?(answer)
      end
    end

    def with_retries
      attempt = 0
      begin
        yield
      rescue *RETRYABLE
        raise if attempt >= retries

        attempt += 1
        @sleeper.call(backoff_for(attempt))
        retry
      end
    end

    def backoff_for(attempt) = (2**attempt) * (0.5 + (rand / 2))
  end
end
