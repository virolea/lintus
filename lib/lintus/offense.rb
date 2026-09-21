# frozen_string_literal: true

module Lintus
  # A rule the model flagged on a file, with the probability it gave.
  Offense = Struct.new(:path, :rule, :noul, keyword_init: true) do
    def message = rule.description

    def to_h
      { path: path, rule: rule.id, message: message, noul: noul.round(3) }
    end
  end
end
