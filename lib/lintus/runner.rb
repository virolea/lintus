# frozen_string_literal: true

module Lintus
  # Sends each file to Jev once, with every rule that applies to it as a
  # separate noul question, and turns the answers into offenses.
  class Runner
    RETRYABLE = [Jev::RateLimitError, Jev::OverloadedError].freeze

    attr_reader :config, :jobs, :retries

    def initialize(config, jobs: 4, retries: 3, sleeper: Kernel.method(:sleep))
      @config = config
      @jobs = [jobs.to_i, 1].max
      @retries = retries
      @sleeper = sleeper
    end

    # Pairs every file with the rules that apply to it, dropping files no rule covers.
    def plan(files)
      files.filter_map do |file|
        rules = config.rules_for(file.path)
        [file, rules] unless rules.empty?
      end
    end

    # Checks the [file, rules] pairs from #plan concurrently.
    def run(tasks)
      report = Report.new
      queue = Queue.new
      tasks.each { |task| queue << task }
      queue.close

      workers = Array.new([jobs, tasks.size].min) do
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
      content = file.content
      if (reason = skip_reason(content))
        report.record_skipped(file.path, reason)
        return
      end

      response = with_retries { perform(file, rules, content) }
      report.record_checked(file.path, offenses_for(file, rules, response.answers))
    rescue Jev::Error, Error => e
      report.record_failure(file.path, e)
    end

    private

    def skip_reason(content)
      return "binary file" if content.include?("\0") || !content.valid_encoding?
      if content.bytesize > config.max_file_size
        return "larger than max_file_size (#{content.bytesize} > #{config.max_file_size} bytes)"
      end

      nil
    end

    def perform(file, rules, content)
      Jev::Query.new("File: #{file.path}\n\n#{content}").perform do |query|
        rules.each { |rule| rule.add_to(query) }
      end
    end

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
