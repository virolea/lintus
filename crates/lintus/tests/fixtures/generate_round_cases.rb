# frozen_string_literal: true

# Writes round_cases.tsv: how Ruby rounds and prints probabilities, as the
# text format (round(2)) and the json format (round(3)) do. One line per value:
# "<f64 bits in hex><TAB><x.round(2).to_s><TAB><x.round(3).to_s>". The Rust
# formatter is tested against it (src/format.rs). Run with any Ruby:
#
#   ruby crates/lintus/tests/fixtures/generate_round_cases.rb

def bits(float) = [float].pack("E").unpack1("Q<")
def next_float(float, direction) = [bits(float) + direction].pack("Q<").unpack1("E")

values = [0.0, 1.0, 0.5, 0.125, 0.995, 0.9995, 0.0049, 0.005, 0.0005, 1e-5, 1e-20, 2.0, 12.345]
(0..1000).each do |k|
  exact = k / 1000.0
  values.push(exact, (k + 0.5) / 1000.0, (k + 0.5) / 100.0)
  values.push(next_float(exact, 1), next_float(exact, -1)) if k.positive?
end
random = Random.new(20_260_923)
5000.times { values << random.rand }

path = File.join(__dir__, "round_cases.tsv")
File.open(path, "w") do |file|
  values.uniq.each { |value| file.puts format("%016x\t%s\t%s", bits(value), value.round(2), value.round(3)) }
end
puts "Wrote #{values.uniq.size} cases to #{path}"
