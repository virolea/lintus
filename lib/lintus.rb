# frozen_string_literal: true

require "zeitwerk"
require "jev"
require "yaml"
require "pathname"
require_relative "lintus/version"

loader = Zeitwerk::Loader.for_gem
loader.inflector.inflect("cli" => "CLI", "github" => "GitHub", "json" => "JSON")
loader.setup

# Lintus turns a YAML file of plain-language rules into a linter: every rule is
# asked to the Jev model as a noul question against each file it applies to, and
# an offense is reported wherever the answer says so.
module Lintus
  class Error < StandardError; end
  class ConfigError < Error; end

  CONFIG_FILENAMES = %w[.lintus.yml lintus.yml .lintus.yaml lintus.yaml].freeze
end
