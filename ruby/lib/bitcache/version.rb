# This is free and unencumbered software released into the public domain.

module Bitcache; end
module Bitcache::VERSION; end

module Bitcache::VERSION
  FILE = File.expand_path('../../../VERSION', __FILE__)
  STRING = File.read(FILE).chomp.freeze
  MAJOR, MINOR, PATCH, EXTRA = STRING.split('.').map(&:freeze)
end # Bitcache::VERSION
