# frozen_string_literal: true

require_relative "lib/lintus/version"

Gem::Specification.new do |spec|
  spec.name = "lintus"
  spec.version = Lintus::VERSION
  spec.authors = ["Vincent Rolea"]
  spec.email = ["3525369+virolea@users.noreply.github.com"]

  spec.summary = "A linter whose rules are plain-language questions, answered by the Jev model."
  spec.description = "Lintus reads linting rules from a YAML file at the root of your repository, " \
                     "asks the Typesafe Jev model each rule as a noul question against every " \
                     "matching file, and reports an offense wherever the model says so. " \
                     "Runs on a whole tree, a git diff, or the staged files of a commit."
  spec.homepage = "https://github.com/virolea/lintus"
  spec.license = "MIT"
  spec.required_ruby_version = ">= 3.2.0"
  spec.metadata["allowed_push_host"] = "https://rubygems.org"
  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/blob/main/CHANGELOG.md"
  spec.metadata["rubygems_mfa_required"] = "true"

  gemspec = File.basename(__FILE__)
  spec.files = IO.popen(%w[git ls-files -z], chdir: __dir__, err: IO::NULL) do |ls|
    ls.readlines("\x0", chomp: true).reject do |f|
      (f == gemspec) ||
        f.start_with?(*%w[bin/ Gemfile .gitignore test/ .github/ .rubocop.yml .lintus.yml])
    end
  end
  spec.bindir = "exe"
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ["lib"]

  spec.add_dependency "jev", "~> 0.2"
  spec.add_dependency "zeitwerk", "~> 2.6"
end
