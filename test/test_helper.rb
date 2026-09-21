# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path("../lib", __dir__)
require "lintus"

require "minitest/autorun"
require "webmock/minitest"
require "tmpdir"
require "stringio"

Jev.api_key = "test-key"

module LintusTestHelpers
  API_URL = Jev::Client::API_URL

  RULES = {
    "no_sleep" => {
      "description" => "Jobs must not sleep.",
      "question" => "Does this file call sleep?",
      "paths" => ["app/jobs/**/*.rb"]
    },
    "documented" => {
      "description" => "Classes carry a comment.",
      "question" => "Is every class documented?",
      "offense_when" => false,
      "threshold" => 0.7
    }
  }.freeze

  private

  def build_config(overrides = {})
    Lintus::Config.new({ "paths" => ["**/*.rb"], "rules" => RULES }.merge(overrides))
  end

  def source(path, content = "class Foo; end\n")
    Lintus::SourceFile.new(path) { content }
  end

  def stub_jev(answers, status: 200, model: "jev-1.13.0")
    body = status == 200 ? { model: model, answers: answers }.to_json : { error: "nope" }.to_json
    stub_request(:post, API_URL).to_return(status: status, body: body)
  end

  def noul(value) = { type: "noul", noul: value }

  # A throwaway git repository with an initial commit, yielded as the working directory.
  def with_git_repo
    Dir.mktmpdir("lintus") do |dir|
      dir = File.realpath(dir)
      Dir.chdir(dir) do
        git "init", "-q"
        git "config", "user.email", "test@example.com"
        git "config", "user.name", "Lintus Test"
        git "config", "commit.gpgsign", "false"
        yield dir
      end
    end
  end

  def git(*args)
    out, status = Open3.capture2e("git", *args)
    raise "git #{args.join(" ")} failed: #{out}" unless status.success?

    out
  end

  def write(path, content = "class Foo; end\n")
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, content)
  end

  def commit_all(message = "commit")
    git "add", "-A"
    git "commit", "-qm", message
  end
end

require "fileutils"
require "open3"
