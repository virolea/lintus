# frozen_string_literal: true

module Lintus
  # A file to lint: a repository-relative path and a lazy reader for its content.
  # The reader is what lets `--staged` lint the index rather than the working tree.
  class SourceFile
    attr_reader :path

    def initialize(path, &reader)
      @path = path
      @reader = reader
    end

    def content
      @content ||= @reader.call.dup.force_encoding(Encoding::UTF_8)
    end

    def size = content.bytesize

    def binary? = content.include?("\0") || !content.valid_encoding?

    def ==(other) = other.is_a?(SourceFile) && other.path == path
    alias eql? ==

    def hash = path.hash

    def to_s = path
  end
end
