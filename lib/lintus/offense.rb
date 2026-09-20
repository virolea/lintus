# frozen_string_literal: true

module Lintus
  # A rule the model flagged on a file, with the probability it gave.
  Offense = Struct.new(:path, :rule, :noul, keyword_init: true) do
    def severity = rule.severity
    def error? = rule.error?
    def message = rule.description

    def to_h
      { path: path, rule: rule.id, severity: severity, message: message, noul: noul.round(3) }
    end
  end
end
