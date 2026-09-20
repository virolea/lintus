# frozen_string_literal: true

require "test_helper"

class TestFormatters < Minitest::Test
  include LintusTestHelpers

  def setup
    config = build_config
    @report = Lintus::Report.new
    @report.record_checked("lib/ok.rb", [])
    @report.record_checked("app/jobs/a_job.rb", [
                             Lintus::Offense.new(path: "app/jobs/a_job.rb", rule: config.rule(:no_sleep), noul: 0.912),
                             Lintus::Offense.new(path: "app/jobs/a_job.rb", rule: config.rule(:documented), noul: 0.3)
                           ])
    @report.record_skipped("lib/blob.rb", "binary file")
    @report.record_failure("lib/bad.rb", Jev::APIError.new(500, "boom"))
  end

  def test_text
    output = render("text")

    assert_equal <<~TEXT, output
      app/jobs/a_job.rb: [documented] Classes carry a comment. (warning, noul 0.3)
      app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (error, noul 0.91)
      lib/blob.rb: skipped, binary file
      lib/bad.rb: failed, Jev API error 500: boom

      2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed
    TEXT
  end

  def test_text_when_clean
    assert_equal "0 files inspected, 0 offenses detected\n", render("text", Lintus::Report.new)
  end

  def test_github_emits_workflow_commands
    lines = render("github").lines(chomp: true)

    assert_equal "::warning file=app/jobs/a_job.rb,title=lintus%3A documented::Classes carry a comment.", lines[0]
    assert_equal "::error file=app/jobs/a_job.rb,title=lintus%3A no_sleep::Jobs must not sleep.", lines[1]
    assert_equal "::notice file=lib/blob.rb,title=lintus::Skipped: binary file", lines[2]
    assert_equal "::error file=lib/bad.rb,title=lintus%3A request failed::Jev API error 500: boom", lines[3]
    assert_equal "2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed", lines[4]
  end

  def test_github_escapes_newlines_and_percent_in_messages
    config = Lintus::Config.new({ "rules" => { "r" => { "question" => "q", "description" => "100%\nsure" } } })
    report = Lintus::Report.new
    report.record_checked("a.rb", [Lintus::Offense.new(path: "a.rb", rule: config.rule(:r), noul: 1.0)])

    assert_equal "::error file=a.rb,title=lintus%3A r::100%25%0Asure\n1 file inspected, 1 offense detected\n",
                 render("github", report)
  end

  def test_json
    data = JSON.parse(render("json"))

    summary = { "files_inspected" => 2, "offenses" => 2, "errors" => 1, "warnings" => 1, "skipped" => 1, "failures" => 1 }
    offense = { "path" => "app/jobs/a_job.rb", "rule" => "no_sleep", "severity" => "error",
                "message" => "Jobs must not sleep.", "noul" => 0.912 }

    assert_equal summary, data["summary"]
    assert_equal offense, data["offenses"].last
    assert_equal [{ "path" => "lib/blob.rb", "reason" => "binary file" }], data["skipped"]
    assert_equal "Jev API error 500: boom", data["failures"].first["error"]
  end

  def test_unknown_format
    assert_raises(Lintus::Error) { Lintus::Formatter.for("xml") }
  end

  private

  def render(format, report = @report)
    io = StringIO.new
    Lintus::Formatter.for(format).new(io).render(report)
    io.string
  end
end
