# frozen_string_literal: true

require "test_helper"

class TestReport < Minitest::Test
  include LintusTestHelpers

  def setup
    @config = build_config
    @report = Lintus::Report.new
  end

  def test_exit_status_by_fail_level
    assert_equal 0, @report.exit_status

    @report.record_offenses([Lintus::Offense.new(path: "a.rb", rule: @config.rule(:documented), noul: 0.1)])
    assert_equal 0, @report.exit_status
    assert_equal 1, @report.exit_status(fail_on: "warning")

    @report.record_offenses([Lintus::Offense.new(path: "a.rb", rule: @config.rule(:no_sleep), noul: 0.9)])
    assert_equal 1, @report.exit_status
    assert_equal 0, @report.exit_status(fail_on: "never")

    @report.record_failure("b.rb", Jev::APIError.new(500, "boom"))
    assert_equal 2, @report.exit_status(fail_on: "never")
  end

  def test_sorted_offenses_are_stable_by_path_then_rule
    @report.record_offenses([
                              Lintus::Offense.new(path: "b.rb", rule: @config.rule(:no_sleep), noul: 0.9),
                              Lintus::Offense.new(path: "a.rb", rule: @config.rule(:no_sleep), noul: 0.9),
                              Lintus::Offense.new(path: "a.rb", rule: @config.rule(:documented), noul: 0.1)
                            ])

    assert_equal([%w[a.rb documented], %w[a.rb no_sleep], %w[b.rb no_sleep]],
                 @report.sorted_offenses.map { |o| [o.path, o.rule.id] })
  end
end
