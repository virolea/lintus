# frozen_string_literal: true

require "test_helper"

class TestFileFinder < Minitest::Test
  include LintusTestHelpers

  def test_all_lists_tracked_and_untracked_but_not_ignored_files
    with_git_repo do |dir|
      write("lib/a.rb")
      write(".gitignore", "ignored.rb\n")
      commit_all
      write("untracked.rb")
      write("ignored.rb")

      assert_equal %w[.gitignore lib/a.rb untracked.rb], Lintus::FileFinder.new(dir).all.map(&:path)
    end
  end

  def test_all_outside_git_globs_the_tree
    Dir.mktmpdir do |dir|
      write(File.join(dir, "a.rb"))
      write(File.join(dir, "sub/.hidden"))

      assert_equal %w[.hidden a.rb].sort, Lintus::FileFinder.new(dir).all.map(&:path).map { File.basename(_1) }.sort
    end
  end

  def test_changed_since_uses_merge_base_and_includes_working_tree_and_untracked
    with_git_repo do |dir|
      write("lib/a.rb")
      write("lib/b.rb")
      commit_all
      git "checkout", "-qb", "feature"
      write("lib/a.rb", "class Foo; def x; end; end\n")
      commit_all "change a"
      git "checkout", "-q", "-"
      write("lib/b.rb", "class Bar; end\n")
      commit_all "change b on main"
      git "checkout", "-q", "feature"
      write("lib/c.rb")
      write("lib/d.rb")
      git "add", "lib/c.rb"

      finder = Lintus::FileFinder.new(dir)
      main = git("rev-parse", "--abbrev-ref", "@{-1}").strip

      assert_equal %w[lib/a.rb lib/c.rb lib/d.rb], finder.changed_since(main).map(&:path)
      assert_equal %w[lib/c.rb lib/d.rb], finder.changed_since("HEAD").map(&:path)
    end
  end

  def test_changed_since_omits_deleted_files
    with_git_repo do |dir|
      write("lib/a.rb")
      commit_all
      File.delete("lib/a.rb")

      assert_empty Lintus::FileFinder.new(dir).changed_since("HEAD")
    end
  end

  def test_staged_reads_index_content_not_working_tree
    with_git_repo do |dir|
      write("lib/a.rb", "staged\n")
      git "add", "lib/a.rb"
      write("lib/a.rb", "working tree\n")
      write("lib/unstaged.rb")

      files = Lintus::FileFinder.new(dir).staged

      assert_equal %w[lib/a.rb], files.map(&:path)
      assert_equal "staged\n", files.first.content
    end
  end

  def test_git_modes_require_a_repository
    Dir.mktmpdir do |dir|
      assert_raises(Lintus::Error) { Lintus::FileFinder.new(dir).staged }
      assert_raises(Lintus::Error) { Lintus::FileFinder.new(dir).changed_since("HEAD") }
    end
  end

  def test_explicit_resolves_files_and_directories_relative_to_the_caller
    Dir.mktmpdir do |dir|
      dir = File.realpath(dir)
      write(File.join(dir, "app/jobs/a.rb"))
      write(File.join(dir, "app/b.rb"))
      write(File.join(dir, "lib/c.rb"))

      files = Lintus::FileFinder.new(dir).explicit(["jobs", "../lib/c.rb", "missing.rb"], from: File.join(dir, "app"))

      assert_equal %w[app/jobs/a.rb lib/c.rb], files.map(&:path)
      assert_raises(Lintus::Error) { Lintus::FileFinder.new(dir).explicit(["/etc/hostname"]) }
    end
  end

  def test_explicit_directories_honour_gitignore_inside_a_repository
    with_git_repo do |dir|
      write("app/a.rb")
      write("app/ignored.rb")
      write(".gitignore", "ignored.rb\n")
      commit_all
      write("app/untracked.rb")

      files = Lintus::FileFinder.new(dir).explicit(["app", "."], from: dir)

      assert_equal %w[.gitignore app/a.rb app/untracked.rb], files.map(&:path)
    end
  end
end
