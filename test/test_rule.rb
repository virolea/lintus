# frozen_string_literal: true

require "test_helper"

class TestRule < Minitest::Test
  include LintusTestHelpers

  def test_defaults
    rule = Lintus::Rule.new("no_sleep", { "question" => "Does this file call sleep?" })

    assert_equal "no_sleep", rule.id
    assert_equal "Does this file call sleep?", rule.description
    assert_equal "error", rule.severity
    assert_equal true, rule.offense_when
    assert_nil rule.threshold
    assert_nil rule.criteria
    assert rule.error?
    assert rule.applies_to?("anything/at/all.txt")
  end

  def test_rule_paths_override_default_paths
    rule = Lintus::Rule.new("r", { "question" => "q", "paths" => "app/**/*.rb" }, default_paths: ["**/*"])

    assert rule.applies_to?("app/x.rb")
    refute rule.applies_to?("lib/x.rb")
  end

  def test_default_paths_apply_when_rule_has_none
    rule = Lintus::Rule.new("r", { "question" => "q" }, default_paths: ["lib/**/*"])

    assert rule.applies_to?("lib/x.rb")
    refute rule.applies_to?("app/x.rb")
  end

  def test_exclude_wins_over_paths
    rule = Lintus::Rule.new("r", { "question" => "q", "paths" => ["**/*.rb"], "exclude" => ["**/*_test.rb"] })

    assert rule.applies_to?("lib/x.rb")
    refute rule.applies_to?("test/x_test.rb")
  end

  def test_symbol_keys_are_accepted
    rule = Lintus::Rule.new(:r, { question: "q", severity: :warning, criteria: { "true" => "yes", "false" => "no" } })

    assert_equal "warning", rule.severity
    assert_equal({ "true" => "yes", "false" => "no" }, rule.criteria)
  end

  def test_add_to_query_forwards_criteria_and_threshold
    rule = Lintus::Rule.new("r",
                            { "question" => "q?", "criteria" => { "true" => "t", "false" => "f" }, "threshold" => 0.8 })
    query = Jev::Query.new("state")
    rule.add_to(query)

    question = query.questions[:r]
    assert_equal({ type: :noul, instructions: "q?", criteria: { "true" => "t", "false" => "f" } }, question.to_h)
    question.answer_with("noul" => 0.75)
    refute question.result
  end

  def test_offense_follows_offense_when
    positive = Lintus::Rule.new("r", { "question" => "q" })
    negative = Lintus::Rule.new("r", { "question" => "q", "offense_when" => false })
    answer = Struct.new(:result).new(true)

    assert positive.offense?(answer)
    refute negative.offense?(answer)
  end

  def test_validation_errors
    assert_config_error("invalid rule id") { Lintus::Rule.new("No-Sleep", { "question" => "q" }) }
    assert_config_error("`question` is required") { Lintus::Rule.new("r", { "description" => "d" }) }
    assert_config_error("expected a map") { Lintus::Rule.new("r", "just a string") }
    assert_config_error("`severity` must be one of") { Lintus::Rule.new("r", { "question" => "q", "severity" => "fatal" }) }
    assert_config_error("`threshold` must be") { Lintus::Rule.new("r", { "question" => "q", "threshold" => 2 }) }
    assert_config_error("`criteria` must be a map") { Lintus::Rule.new("r", { "question" => "q", "criteria" => "x" }) }
    assert_config_error("`offense_when` must be") { Lintus::Rule.new("r", { "question" => "q", "offense_when" => "yes" }) }
  end

  private

  def assert_config_error(message, &)
    error = assert_raises(Lintus::ConfigError, &)
    assert_includes error.message, message
  end
end
