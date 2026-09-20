# frozen_string_literal: true

module Lintus
  # Small checks shared by everything that reads the config file.
  module Schema
    module_function

    def reject_unknown_keys!(hash, allowed, context:)
      unknown = hash.keys - allowed
      return if unknown.empty?

      raise ConfigError, "#{context}: unknown key(s) #{unknown.join(", ")} (expected #{allowed.join(", ")})"
    end

    def string_list(value) = Array(value).map(&:to_s)
  end
end
