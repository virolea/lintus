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

    # Every file in the tree: git's view when inside a repository, else a walk.
    def all = sources(files_under(root))

    def changed_since(ref)
      require_repository!
      sources(git.changed_files_since(ref))
    end

    def staged
      require_repository!
      sources(git.staged_files, existing_only: false) { |path| git.staged_content(path) }
    end

    # Paths given on the command line, resolved against the current directory.
    def explicit(paths, from: Dir.pwd)
      sources(paths.flat_map { |path| files_under(File.expand_path(path, from)) })
    end

    private

    def require_repository!
      raise Error, "#{root} is not a git repository: --diff and --staged need git" unless git.repository?
    end

    # Repository-relative paths of the files at or beneath an absolute path.
    def files_under(absolute)
      return [relativize(absolute)] unless File.directory?(absolute)

      relative = relativize(absolute)
      return git.files(relative) if git.repository?

      Dir.glob("**/*", File::FNM_DOTMATCH, base: absolute)
         .reject { |path| path == ".git" || path.start_with?(".git/") }
         .map { |path| relative == "." ? path : File.join(relative, path) }
    end

    def sources(paths, existing_only: true, &reader)
      reader ||= ->(path) { File.binread(File.join(root, path)) }

      paths.sort.uniq.filter_map do |path|
        next if existing_only && !File.file?(File.join(root, path))

        SourceFile.new(path) { reader.call(path) }
      end
    end

    def relativize(absolute)
      relative = Pathname.new(absolute).relative_path_from(Pathname.new(root)).to_s
      raise Error, "#{absolute} is outside the config root #{root}" if relative.start_with?("..")

      relative
    end
  end
end
