# frozen_string_literal: true

require "test_helper"

class TestConfig < Minitest::Test
  include LintusTestHelpers

  def test_rules_for_applies_global_exclude_then_rule_globs
    config = build_config("exclude" => ["vendor/**"])

    assert_equal %w[no_sleep documented], config.rules_for("app/jobs/a_job.rb").map(&:id)
    assert_equal %w[documented], config.rules_for("lib/a.rb").map(&:id)
    assert_empty config.rules_for("vendor/gem/a.rb")
    assert_empty config.rules_for("README.md")
  end

  def test_rule_lookup_and_defaults
    config = build_config

    assert_equal "no_sleep", config.rule(:no_sleep).id
    assert_equal Lintus::Config::DEFAULT_MAX_FILE_SIZE, config.max_file_size
  end

  def test_load_walks_up_from_a_subdirectory
    Dir.mktmpdir do |dir|
      write(File.join(dir, ".lintus.yml"), "rules:\n  r:\n    question: q\n")
      FileUtils.mkdir_p(File.join(dir, "a/b"))

      config = Lintus::Config.load(dir: File.join(dir, "a/b"))

      assert_equal File.realpath(dir), File.realpath(config.root)
      assert_equal %w[r], config.rules.map(&:id)
    end
  end

  def test_load_with_explicit_path_uses_its_directory_as_root
    Dir.mktmpdir do |dir|
      path = File.join(dir, "custom.yml")
      write(path, "rules:\n  r:\n    question: q\n")

      config = Lintus::Config.load(path, dir: "/")

      assert_equal File.expand_path(dir), config.root
    end
  end

  def test_load_errors
    Dir.mktmpdir do |dir|
      error = assert_raises(Lintus::ConfigError) { Lintus::Config.load(dir: dir) }
      assert_includes error.message, "No config file found"

      assert_raises(Lintus::ConfigError) { Lintus::Config.load(File.join(dir, "missing.yml")) }

      write(File.join(dir, ".lintus.yml"), "rules: [\n")
      error = assert_raises(Lintus::ConfigError) { Lintus::Config.load(dir: dir) }
      assert_includes error.message, "not valid YAML"
    end
  end

  def test_validation
    assert_raises(Lintus::ConfigError) { Lintus::Config.new([]) }
    assert_raises(Lintus::ConfigError) { Lintus::Config.new({ "rules" => {} }) }
    assert_raises(Lintus::ConfigError) { Lintus::Config.new({ "rules" => [] }) }
    assert_raises(Lintus::ConfigError) { build_config("max_file_size" => "big") }

    error = assert_raises(Lintus::ConfigError) { build_config("rule" => {}) }
    assert_includes error.message, "config: unknown key(s) rule"

    error = assert_raises(Lintus::ConfigError) { build_config("rules" => { "r" => { "question" => "q", "severty" => "x" } }) }
    assert_includes error.message, "rule r: unknown key(s) severty"
  end
end
