# See: https://docs.ruby-lang.org/en/4.0/Gem/Specification.html

require 'distrib/ruby/gemspec'

Distrib::Ruby::Gemspec.build!(__FILE__) do |gemspec|
  gemspec.summary     = "Bitcache for Ruby"
  gemspec.description = "Bitcache is a distributed content-addressable storage (CAS) system."
  gemspec.homepage    = "https://bitcache.dev"
  gemspec.metadata    = {
    :source_code_uri  => "https://github.com/artob/bitcache",
    :bug_tracker_uri  => "https://github.com/artob/bitcache/issues",
    :changelog_uri    => "https://github.com/artob/bitcache/blob/master/CHANGES.md",
  }.transform_keys(&:to_s)
end
