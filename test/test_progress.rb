# frozen_string_literal: true

require "test_helper"

class TestProgress < Minitest::Test
  class Terminal < StringIO
    def tty? = true
  end

  def test_on_a_terminal_redraws_in_place_and_clears_when_done
    io = Terminal.new
    progress = Lintus::Progress.new(io, 12)

    progress.start
    progress.update(1)
    progress.update(12)
    progress.finish

    assert_equal ["Checking 0/12 files", "Checking 1/12 files", "Checking 12/12 files", " " * 20],
                 io.string.split("\r").reject(&:empty?)
    assert io.string.end_with?("\r")
  end

  def test_off_a_terminal_prints_one_line_and_never_redraws
    io = StringIO.new
    progress = Lintus::Progress.new(io, 3)

    progress.start
    progress.update(1)
    progress.update(3)
    progress.finish

    assert_equal "Checking 3 files...\n", io.string
  end

  def test_says_nothing_when_there_is_nothing_to_check
    io = Terminal.new
    progress = Lintus::Progress.new(io, 0)

    progress.start
    progress.finish

    assert_empty io.string
  end
end
