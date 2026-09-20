# frozen_string_literal: true

require "open3"

module Lintus
  # A thin wrapper around the git commands file discovery needs.
  class Git
    class CommandError < Error; end

    CHANGE_FILTER = "--diff-filter=ACMR"

    attr_reader :root

    def initialize(root)
      @root = root
    end

    def repository? = run("rev-parse", "--is-inside-work-tree", allow_failure: true) == "true"

    def tracked_files = lines("ls-files", "-z", "--cached", "--exclude-standard")

    def untracked_files = lines("ls-files", "-z", "--others", "--exclude-standard")

    def staged_files = lines("diff", "-z", "--cached", "--name-only", CHANGE_FILTER)

    # Files that differ between the working tree and the merge base of `ref` and HEAD.
    # With `ref` at HEAD this is "what I have not committed yet".
    def changed_files_since(ref)
      base = run("merge-base", ref, "HEAD", allow_failure: true) || ref
      lines("diff", "-z", "--name-only", CHANGE_FILTER, base)
    end

    def staged_content(path) = run("show", ":#{path}", chomp: false)

    private

    def lines(*)
      run(*, chomp: false).split("\0").reject(&:empty?)
    end

    def run(*args, allow_failure: false, chomp: true)
      stdout, stderr, status = Open3.capture3("git", *args, chdir: root)

      unless status.success?
        return nil if allow_failure

        raise CommandError, "git #{args.join(" ")} failed: #{stderr.strip}"
      end

      chomp ? stdout.chomp : stdout
    end
  end
end
