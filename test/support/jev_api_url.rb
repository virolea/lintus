# frozen_string_literal: true

# Loaded through RUBYOPT by test/support/lintus-ruby: points the Jev client at
# $JEV_API_URL so the conformance suite can run the Ruby lintus against a fake server.
require "jev"

if (url = ENV.fetch("JEV_API_URL", nil))
  Jev::Client.send(:remove_const, :API_URL)
  Jev::Client.const_set(:API_URL, url)
end
