#!/usr/bin/env ruby

require 'minitest/autorun'
require 'ostruct'

# Temporary build directory.
dir = ARGV[0]

# Compile extension into shared object.
Dir.chdir(dir) do
  `ruby -r mkmf -e 'create_makefile("stache")'`
  `make`
end

# Load compiled extension.
require "#{dir}/stache"

class Robot
  attr_reader :name

  def initialize(login:)
    @name = { 'login' => login }
  end

  def bio
    { 'html' => '<p>A customizable, life embetterment robot.</p>' }
  end

  def disposition(a, b, c)
    'friendly'
  end
end

describe Stache do
  subject { Stache::Templates.new }

  describe 'variable tag' do
    it 'replaces with hash context' do
      context = {
        'name' => {
          'login' => 'hubot',
          'real' => 'Hubot'
        },
        'bio' => {
          'html' => '<p>A customizable, life embetterment robot.</p>'
        }
      }
      value = subject.robot(context)
      assert_match /<strong>hubot<\/strong>/, value
      assert_match /Hubot/, value
      assert_match /<p>A customizable/, value
    end

    it 'replaces only defined hash keys' do
      context = Hash.new('default')
      value = subject.robot(context)
      assert_match /<strong><\/strong>/, value
      refute_match /default/, value
    end

    it 'replaces hash symbol keys' do
      skip
      context = { name: { login: 'hubot' } }
      value = subject.robot(context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with struct context' do
      skip
      Context = Struct.new(:name)
      context = Context.new({ 'login' => 'hubot' })
      value = subject.robot(context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with open struct context' do
      context = OpenStruct.new
      context.name = { 'login' => 'hubot' }
      value = subject.robot(context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with object attribute' do
      context = Robot.new(login: 'hubot')
      value = subject.robot(context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with object method' do
      context = Robot.new(login: 'hubot')
      value = subject.robot(context)
      assert_match /<p>A customizable/, value
    end

    it 'does not replace with method requiring arguments' do
      context = Robot.new(login: 'hubot')
      value = subject.robot(context)
      refute_match /friendly/, value
    end
  end

  describe 'escaping special characters' do
    it 'escapes characters in template text' do
      value = subject.escape({})
      assert_equal "<kbd>\" \\n \"</kbd>\n", value
    end
  end
end
