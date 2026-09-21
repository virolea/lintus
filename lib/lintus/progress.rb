# frozen_string_literal: true

module Lintus
  # Tells the person waiting that something is happening. On a terminal it is a
  # counter updated in place and erased when done; elsewhere (CI logs, pipes) it
  # is a single line announcing the run, so nothing garbles captured output.
  class Progress
    def initialize(io, total)
      @io = io
      @total = total
      @tty = io.respond_to?(:tty?) && io.tty?
      @width = 0
    end

    def start
      return if @total.zero?

      if @tty
        draw(0)
      else
        @io.puts("Checking #{@total} #{@total == 1 ? "file" : "files"}...")
      end
    end

    def update(done)
      draw(done) if @tty
    end

    def finish
      return unless @tty && @width.positive?

      @io.print "\r#{" " * @width}\r"
      @io.flush
    end

    private

    def draw(done)
      line = "Checking #{done}/#{@total} files"
      @width = [@width, line.size].max
      @io.print "\r#{line.ljust(@width)}"
      @io.flush
    end
  end
end
