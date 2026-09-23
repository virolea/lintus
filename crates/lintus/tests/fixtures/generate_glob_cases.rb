# frozen_string_literal: true

# Writes glob_cases.tsv: whether the Ruby lintus matched each pattern against
# each path, one "pattern<TAB>path<TAB>0|1" line per pair. The Rust matcher is
# tested against it (src/glob.rs). Run from the repository root while the Ruby
# implementation is still there:
#
#   ruby -Ilib crates/lintus/tests/fixtures/generate_glob_cases.rb
require "lintus"

PATTERNS = [
  "*", "**", "**/*", "*.rb", "**/*.rb", "app/**/*.rb", "app/*.rb", "app/**", "app/**/", "vendor", "vendor/",
  "./vendor", "./vendor/**", "vendor/*", "**/vendor/**", "**/*_test.rb", "test/**/*_test.rb",
  "{app,lib}/**/*.rb", "**/*.{yml,yaml}", "*.{rb,rake}", "{a,b", "a}", "{a,{b,c}}/*.rb", "{,lib/}x.rb",
  "lib/[a-c]*.rb", "lib/[!a-c]*.rb", "lib/[^a]*.rb", "lib/[a-]*.rb", "lib/[]]*.rb", "lib/[", "lib/[a",
  "lib/?.rb", "lib/??.rb", "?", ".*", ".*/**", "**/.*", ".github/**/*.yml", "**/**/*.rb", "a/**/**/b.rb",
  "a/**/b/**/c.rb", "**/b", "a*b*c.rb", "*a*", "lib/\\*.rb", "lib\\/x.rb", "lib/\\[.rb", "Gemfile",
  "Gemfile*", "db/schema.rb", "db", "db/", "exe/*", "*/", "a/**/*/c.rb", "**/*/", "lib/x.rb/", "*.RB",
  "é*.rb", "**/é.rb", "[é]*.rb", "a/*/c.rb", "**", "**/", "a/**", "*/*.rb", "**/x.rb", "{}x.rb",
  "lib/{a,b}.rb", "lib/{a,}b.rb", "{app,{lib,test}}", "\\{a}.rb", "**/{a,b}/**/*.rb"
].freeze

PATHS = [
  "a.rb", "app/a.rb", "app/jobs/a_job.rb", "app/jobs/deep/x.rb", "lib/a.rb", "lib/b.rb", "lib/ab.rb",
  "lib/d.rb", "lib/-.rb", "lib/].rb", "lib/*.rb", "lib/x.rb", "lib/[.rb", "vendor/x.rb",
  "vendor/gem/lib/x.rb", "vendored/x.rb", "vendor", "test/a_test.rb", "test/unit/b_test.rb", "a_test.rb",
  ".github/workflows/ci.yml", ".hidden", ".lintus.yml", "config/a.yaml", "config/deep/b.yml", "Gemfile",
  "Gemfile.lock", "sub/Gemfile", "db/schema.rb", "db/migrate/1.rb", "exe/lintus", "a/b.rb", "a/x/b.rb",
  "a/x/y/b.rb", "a/b/c.rb", "a/x/b/y/c.rb", "abc.rb", "aXbYc.rb", "x.RB", "x.rb", "é.rb", "dir/é.rb",
  "éa.rb", "x.rake", "b/x.rb", "a/c.rb", "a/b/x/c.rb", "{a}.rb", "b.rb", "a", "b", "lib/a/b.rb",
  "test/lib/x.rb", ".a/.b/c.rb"
].freeze

path = File.join(__dir__, "glob_cases.tsv")
File.open(path, "w") do |file|
  PATTERNS.each do |pattern|
    PATHS.each do |candidate|
      file.puts [pattern, candidate, Lintus::Glob.match?(pattern, candidate) ? 1 : 0].join("\t")
    end
  end
end
puts "Wrote #{PATTERNS.size * PATHS.size} cases to #{path}"
