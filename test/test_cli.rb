# frozen_string_literal: true

require "test_helper"

class TestCLI < Minitest::Test
  include LintusTestHelpers

  CONFIG = <<~YAML
    paths: ["**/*.rb"]
    rules:
      no_sleep:
        description: Jobs must not sleep.
        question: Does this file call sleep?
        paths: ["app/jobs/**/*.rb"]
      documented:
        question: Is every class documented?
        offense_when: false
        severity: warning
  YAML

  def test_init_writes_a_loadable_config_and_refuses_to_overwrite
    Dir.mktmpdir do |dir|
      assert_equal 0, run_cli(%w[init], dir: dir)
      assert_includes @stdout.string, "Wrote"

      config = Lintus::Config.load(dir: dir)
      assert_equal 3, config.rules.size

      assert_equal 2, run_cli(%w[init], dir: dir)
      assert_includes @stderr.string, "already exists"
    end
  end

  def test_version_and_help
    assert_equal 0, run_cli(%w[--version])
    assert_equal "lintus #{Lintus::VERSION}\n", @stdout.string

    assert_equal 0, run_cli(%w[--help])
    assert_includes @stdout.string, "Usage: lintus"
  end

  def test_list_does_not_need_an_api_key
    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)
      write("app/jobs/a_job.rb")
      write("lib/b.rb")
      write("README.md", "hi")

      assert_equal 0, run_cli(%w[--list], dir: dir, env: {})
      assert_equal "app/jobs/a_job.rb: no_sleep, documented\nlib/b.rb: documented\n2 file(s) would be checked\n",
                   @stdout.string
    end
  end

  def test_missing_api_key_is_a_usage_error
    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)
      write("lib/b.rb")

      assert_equal 2, run_cli([], dir: dir, env: {})
      assert_includes @stderr.string, "JEV_API_KEY"
    end
  end

  def test_full_run_reports_offenses_and_exit_status
    stub_jev({ "no_sleep" => noul(0.95), "documented" => noul(0.9) })

    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)
      write("app/jobs/a_job.rb", "sleep 1\n")

      assert_equal 1, run_cli([], dir: dir)
      assert_equal <<~TEXT, @stdout.string
        app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (error, noul 0.95)

        1 file inspected, 1 offense detected
      TEXT

      assert_equal 0, run_cli(%w[--fail-on never], dir: dir)
      assert_equal 1, run_cli(%w[--format json --api-key from-flag], dir: dir, env: {})
      assert_equal 1, JSON.parse(@stdout.string)["summary"]["offenses"]
    end
  end

  def test_diff_and_staged_and_explicit_files
    stub_jev({ "documented" => noul(0.9) })

    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)
      write("lib/old.rb")
      commit_all
      write("lib/new.rb")
      git "add", "lib/new.rb"
      write("lib/untracked.rb")

      assert_equal 0, run_cli(%w[--diff --list], dir: dir)
      assert_includes @stdout.string, "2 file(s) would be checked"

      assert_equal 0, run_cli(%w[--staged --list], dir: dir)
      assert_equal "lib/new.rb: documented\n1 file(s) would be checked\n", @stdout.string

      assert_equal 0, run_cli(%w[lib/old.rb --list], dir: dir)
      assert_equal "lib/old.rb: documented\n1 file(s) would be checked\n", @stdout.string

      assert_equal 0, run_cli(%w[--staged], dir: dir)
      assert_equal "1 file inspected, 0 offenses detected\n", @stdout.string
    end
  end

  def test_conflicting_and_invalid_arguments
    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)

      assert_equal 2, run_cli(%w[--staged lib/a.rb], dir: dir)
      assert_includes @stderr.string, "cannot be combined"

      assert_equal 2, run_cli(%w[--format xml], dir: dir)
      assert_equal 2, run_cli(%w[init extra], dir: dir)
      assert_equal 2, run_cli(%w[--config nope.yml], dir: dir)
      assert_includes @stderr.string, "nope.yml"
    end
  end

  def test_github_format_is_the_default_under_actions
    stub_jev({ "documented" => noul(0.1) })

    with_git_repo do |dir|
      write(".lintus.yml", CONFIG)
      write("lib/b.rb")

      run_cli([], dir: dir, env: { "JEV_API_KEY" => "k", "GITHUB_ACTIONS" => "true" })

      assert_includes @stdout.string, "::warning file=lib/b.rb,title=lintus%3A documented::Is every class documented?"
    end
  end

  private

  def run_cli(argv, dir: Dir.pwd, env: { "JEV_API_KEY" => "test-key" })
    @stdout = StringIO.new
    @stderr = StringIO.new
    Lintus::CLI.new(stdout: @stdout, stderr: @stderr, env: env, dir: dir).run(argv)
  end
end
