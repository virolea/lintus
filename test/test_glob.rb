# frozen_string_literal: true

require "test_helper"

class TestGlob < Minitest::Test
  def test_double_star_spans_directories
    assert Lintus::Glob.match?("app/**/*.rb", "app/jobs/foo_job.rb")
    assert Lintus::Glob.match?("app/**/*.rb", "app/foo.rb")
    refute Lintus::Glob.match?("app/**/*.rb", "lib/foo.rb")
  end

  def test_bare_directory_matches_everything_beneath_it
    assert Lintus::Glob.match?("vendor", "vendor/gem/lib/x.rb")
    assert Lintus::Glob.match?("vendor/", "vendor/x.rb")
    assert Lintus::Glob.match?("./vendor", "vendor/x.rb")
    refute Lintus::Glob.match?("vendor", "vendored/x.rb")
  end

  def test_trailing_double_star_matches_nested_files
    assert Lintus::Glob.match?("vendor/**", "vendor/a/b/c.rb")
    assert Lintus::Glob.match?("vendor/**", "vendor/c.rb")
  end

  def test_exact_file
    assert Lintus::Glob.match?("Gemfile", "Gemfile")
    refute Lintus::Glob.match?("Gemfile", "sub/Gemfile")
  end

  def test_braces_and_dotfiles
    assert Lintus::Glob.match?("**/*.{yml,yaml}", "config/a.yaml")
    assert Lintus::Glob.match?("**/*.yml", ".github/workflows/ci.yml")
  end

  def test_single_star_does_not_cross_directories
    refute Lintus::Glob.match?("app/*.rb", "app/jobs/foo.rb")
  end
end
