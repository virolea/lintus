# frozen_string_literal: true

module Lintus
  # Resolves which files a run should look at, always as paths relative to the
  # config root with forward slashes, in a stable order.
  class FileFinder
    attr_reader :root, :git

    def initialize(root, git: Git.new(root))
      @root = root
      @git = git
    end

    # Every file in the tree: git's view when inside a repository, else a glob.
    def all
      paths = git.repository? ? git.tracked_files + git.untracked_files : glob_everything
      sources(paths)
    end

    def changed_since(ref)
      require_repository!
      sources(git.changed_files_since(ref) + git.untracked_files)
    end

    def staged
      require_repository!
      git.staged_files.sort.uniq.map { |path| SourceFile.new(path) { git.staged_content(path) } }
    end

    # Paths given on the command line, resolved against the current directory.
    def explicit(paths, from: Dir.pwd)
      expanded = paths.flat_map do |path|
        absolute = File.expand_path(path, from)
        if File.directory?(absolute)
          Dir.glob("**/*", File::FNM_DOTMATCH, base: absolute).map do |f|
            File.join(absolute, f)
          end
        else
          [absolute]
        end
      end

      sources(expanded.select { |absolute| File.file?(absolute) }.map { |absolute| relativize(absolute) })
    end

    private

    def require_repository!
      raise Error, "#{root} is not a git repository: --diff and --staged need git" unless git.repository?
    end

    def glob_everything
      Dir.glob("**/*", File::FNM_DOTMATCH, base: root).reject { |path| path.start_with?(".git/") || path == ".git" }
    end

    def sources(paths)
      paths.sort.uniq.filter_map do |path|
        absolute = File.join(root, path)
        next unless File.file?(absolute)

        SourceFile.new(path) { File.binread(absolute) }
      end
    end

    def relativize(absolute)
      relative = Pathname.new(absolute).relative_path_from(Pathname.new(root)).to_s
      raise Error, "#{absolute} is outside the config root #{root}" if relative.start_with?("..")

      relative
    end
  end
end
