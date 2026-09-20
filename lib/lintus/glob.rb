# frozen_string_literal: true

module Lintus
  # Matches repository-relative paths against the globs used in the config file.
  #
  # Patterns follow Ruby's File.fnmatch with pathname semantics ("**/" spans
  # directories, "{a,b}" alternation is allowed), with two conveniences:
  # a bare directory ("vendor" or "vendor/") matches everything beneath it,
  # and a trailing "**" ("vendor/**") does the same.
  module Glob
    FLAGS = File::FNM_PATHNAME | File::FNM_EXTGLOB | File::FNM_DOTMATCH

    module_function

    def match?(pattern, path)
      expansions(pattern).any? { |expanded| File.fnmatch?(expanded, path, FLAGS) }
    end

    def match_any?(patterns, path)
      patterns.any? { |pattern| match?(pattern, path) }
    end

    def expansions(pattern)
      pattern = pattern.to_s.delete_prefix("./").delete_suffix("/")

      if pattern.end_with?("**")
        [pattern, "#{pattern}/*"]
      elsif pattern.match?(/[*?\[{]/)
        [pattern]
      else
        [pattern, "#{pattern}/**/*"]
      end
    end
  end
end
