# frozen_string_literal: true

module Lintus
  # A file to lint: a repository-relative path and a reader for its content.
  # The reader is what lets `--staged` lint the index rather than the working tree.
  class SourceFile
    attr_reader :path

    def initialize(path, &reader)
      @path = path
      @reader = reader
    end

    # Reads the content afresh on every call so a run holds only the files in flight.
    def content
      (+@reader.call).force_encoding(Encoding::UTF_8)
    rescue SystemCallError, Git::CommandError => e
      raise ReadError, "could not read #{path}: #{e.message}"
    end
  end
end
