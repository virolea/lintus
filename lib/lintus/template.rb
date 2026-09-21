# frozen_string_literal: true

module Lintus
  # The starter config written by `lintus init`.
  module Template
    CONFIG = <<~YAML
      # Lintus configuration. Every rule is a question asked of the Jev model
      # about each matching file. Answer "true" means the file is an offense.
      #
      # Top-level `paths` and `exclude` apply to every rule; a rule can narrow
      # them with its own `paths` and `exclude`. Globs are relative to this file.

      paths:
        - "**/*.rb"

      exclude:
        - "vendor/**"
        - "node_modules/**"
        - "tmp/**"

      # Files larger than this (in bytes) are skipped rather than sent to the model.
      max_file_size: 100000

      rules:
        no_debugging_leftovers:
          description: Debugging statements must not be committed.
          question: Does this file contain leftover debugging code, such as binding.irb, debugger, byebug, or a puts/p/pp used for debugging?
          criteria:
            "true": A debugger breakpoint or a throwaway print statement is present.
            "false": Any output is deliberate program behaviour, or there is none.

        no_hardcoded_secrets:
          description: Secrets must come from the environment or a credentials store, never source code.
          question: Does this file contain a hardcoded secret, such as an API key, password, or private token?
          criteria:
            "true": A literal credential value is written in the source.
            "false": Credentials are read from configuration, or there are none.
          threshold: 0.7

        methods_are_documented:
          description: Public classes should carry a short comment explaining their purpose.
          question: Does every class or module defined in this file have a comment describing its responsibility?
          # This rule is phrased positively, so the offense is a "false" answer.
          offense_when: false
    YAML
  end
end
