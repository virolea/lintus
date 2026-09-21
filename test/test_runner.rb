# frozen_string_literal: true

require "test_helper"

class TestRunner < Minitest::Test
  include LintusTestHelpers

  def setup
    @config = build_config
    @runner = Lintus::Runner.new(@config, jobs: 2, sleeper: ->(seconds) { (@slept ||= []) << seconds })
  end

  def test_plan_pairs_files_with_their_rules_and_drops_uncovered_files
    plan = @runner.plan([source("app/jobs/a_job.rb"), source("lib/b.rb"), source("README.md")])

    assert_equal([["app/jobs/a_job.rb", %w[no_sleep documented]], ["lib/b.rb", %w[documented]]],
                 plan.map { |file, rules| [file.path, rules.map(&:id)] })
  end

  def test_sends_one_query_per_file_with_every_applicable_rule
    stub_jev({ "no_sleep" => noul(0.9), "documented" => noul(0.2) })

    report = lint([source("app/jobs/a_job.rb", "sleep 1\n")])

    assert_requested(:post, API_URL, times: 1) do |request|
      body = JSON.parse(request.body)
      assert_equal "File: app/jobs/a_job.rb\n\nsleep 1\n", body["state"]
      assert_equal %w[no_sleep documented], body["questions"].keys
      assert_equal({ "type" => "noul", "instructions" => "Does this file call sleep?" }, body["questions"]["no_sleep"])
    end

    assert_equal ["app/jobs/a_job.rb"], report.checked.map(&:path)
    assert_equal({ "no_sleep" => 0.9, "documented" => 0.2 }, report.checked.first.answers)
    assert_equal([["no_sleep", 0.9], ["documented", 0.2]], report.offenses.map { |o| [o.rule.id, o.noul] })
  end

  def test_thresholds_and_offense_when_are_honoured
    stub_jev({ "no_sleep" => noul(0.4), "documented" => noul(0.75) })

    report = lint([source("app/jobs/a_job.rb")])

    assert_empty report.offenses
  end

  def test_only_asks_the_rules_that_apply
    stub_jev({ "documented" => noul(0.9) })

    report = lint([source("lib/b.rb")])

    assert_requested(:post, API_URL) { |request| JSON.parse(request.body)["questions"].keys == %w[documented] }
    assert_empty report.offenses
  end

  def test_skips_binary_and_oversized_files_without_calling_the_api
    stub_jev({ "documented" => noul(0.1) })
    runner = Lintus::Runner.new(build_config("max_file_size" => 10))

    report = lint([source("lib/bin.rb", "ab\0cd"), source("lib/big.rb", "x" * 11), source("lib/ok.rb", "x")], runner: runner)

    assert_requested(:post, API_URL, times: 1) { |request| JSON.parse(request.body)["state"].include?("lib/ok.rb") }
    assert_equal ["lib/bin.rb", "lib/big.rb"], report.skipped.map(&:path)
    assert_includes report.skipped.last.reason, "max_file_size"
    assert_equal ["lib/ok.rb"], report.checked.map(&:path)
  end

  def test_retries_rate_limits_with_backoff_then_succeeds
    stub_request(:post, API_URL)
      .to_return(status: 429, body: "slow down").times(2)
      .then.to_return(status: 200, body: { model: "jev-1", answers: { "documented" => noul(0.9) } }.to_json)

    report = lint([source("lib/b.rb")])

    assert_equal 2, @slept.size
    assert @slept.first < @slept.last
    assert_equal 1, report.checked.size
    assert_empty report.failures
  end

  def test_gives_up_after_retries_and_records_a_failure
    stub_request(:post, API_URL).to_return(status: 529, body: "overloaded")
    runner = Lintus::Runner.new(@config, retries: 1, sleeper: ->(_) {})

    report = lint([source("lib/b.rb")], runner: runner)

    assert_equal 1, report.failures.size
    assert_kind_of Jev::OverloadedError, report.failures.first.error
    assert_requested :post, API_URL, times: 2
  end

  def test_other_api_errors_are_not_retried
    stub_request(:post, API_URL).to_return(status: 401, body: "bad key")

    report = lint([source("lib/b.rb")])

    assert_requested :post, API_URL, times: 1
    assert_kind_of Jev::AuthenticationError, report.failures.first.error
    assert_equal 2, report.exit_status
  end

  def test_runs_files_concurrently
    threads = Queue.new
    stub_request(:post, API_URL).to_return do
      threads << Thread.current
      sleep 0.02 # hold the request so the workers overlap
      { status: 200, body: { model: "jev-1", answers: { "documented" => noul(0.9) } }.to_json }
    end

    report = lint(Array.new(6) { |i| source("lib/f#{i}.rb") }, runner: Lintus::Runner.new(@config, jobs: 3))

    assert_equal 6, report.checked.size
    assert_operator Array.new(threads.size) { threads.pop }.uniq.size, :>, 1
  end

  def test_reports_progress_after_each_file
    stub_jev({ "documented" => noul(0.1) })
    seen = []
    runner = Lintus::Runner.new(@config, jobs: 2)

    runner.run(runner.plan(Array.new(4) { |i| source("lib/f#{i}.rb") })) { |done| seen << done }

    assert_equal [1, 2, 3, 4], seen
  end

  def test_unreadable_files_are_recorded_as_failures
    file = Lintus::SourceFile.new("lib/gone.rb") { File.binread("/nonexistent/gone.rb") }

    report = lint([file])

    assert_kind_of Lintus::ReadError, report.failures.first.error
    assert_requested :post, API_URL, times: 0
  end

  private

  def lint(files, runner: @runner) = runner.run(runner.plan(files))
end
