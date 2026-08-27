abort("Expected Ruby 3.4+, but got #{RUBY_VERSION}.") if RUBY_VERSION < '3.4.0'

require 'pathname'

READMER_INPUTS = Dir['data/readmer/*.sh-session'].sort

task default: %w[codegen]

task codegen: READMER_INPUTS

READMER_INPUTS.each do |path|
  desc "Generate #{path}"
  file path do |t|
    command = Pathname(t.name).basename('.*').to_s.split('-').push('--help').join(' ')
    File.open(t.name, 'w') do |f|
      f.puts "$ #{command}"
      f.puts `#{command} 2>&1`.gsub(/\s+$/, "\n")
    end
  end
end

task readme: READMER_INPUTS do |t|
  t.prerequisites.each do |p|
    command = Pathname(p).basename('.*').to_s.split('-').join(' ')
    puts "\n#### `#{command}`\n\n{% render '#{p}' %}\n"
  end
end
