#!/usr/bin/env ruby

require 'minitest/autorun'
require 'ostruct'

# Temporary build directory.
dir = ARGV[0]

# Compile extension into shared object.
Dir.chdir(dir) do
  `ruby -r mkmf -e '$CFLAGS = "-std=c99"; create_makefile("stache")'`
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

  def disposition
    'friendly'
  end
end

class ArgumentativeRobot < Robot
  def disposition(a, b, c)
    'should raise'
  end
end

describe Stache do
  subject { Stache::Templates }

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
      value = subject.render('robot', context)
      assert_match /<strong>hubot<\/strong>/, value
      assert_match /Hubot/, value
      assert_match /<p>A customizable/, value
    end

    it 'replaces only defined hash keys' do
      context = Hash.new('default')
      value = subject.render('robot', context)
      assert_match /<strong><\/strong>/, value
      refute_match /default/, value
    end

    it 'replaces hash symbol keys' do
      context = { name: { login: 'hubot' } }
      value = subject.render('robot', context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with struct context' do
      context = Struct.new(:name).new({ 'login' => 'hubot' })
      value = subject.render('robot',context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with open struct context' do
      context = OpenStruct.new
      context.name = { 'login' => 'hubot' }
      value = subject.render('robot', context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with object attribute' do
      context = Robot.new(login: 'hubot')
      value = subject.render('robot', context)
      assert_match /<strong>hubot<\/strong>/, value
    end

    it 'replaces with object method' do
      context = Robot.new(login: 'hubot')
      value = subject.render('robot', context)
      assert_match /<p>A customizable/, value
    end

    it 'raises calling method with arguments' do
      context = ArgumentativeRobot.new(login: 'hubot')
      assert_raises(ArgumentError) do
        subject.render('robot', context)
      end
    end
  end

  describe 'fetching a key from a type' do
    it 'does not replace with nil method' do
      context = { value: nil }
      value = subject.render('types/nil', context)
      assert_equal '', value.strip
    end

    it 'replaces with float method' do
      context = { value: -42.0 }
      value = subject.render('types/float', context)
      assert_equal '42.0', value.strip
    end

    it 'replaces with string method' do
      context = { value: 'CAPS' }
      value = subject.render('types/string', context)
      assert_equal 'caps', value.strip
    end

    it 'replaces with class method' do
      context = { value: String }
      value = subject.render('types/class', context)
      assert_equal 'Object', value.strip
    end

    it 'replaces with array method' do
      context = { value: [42] }
      value = subject.render('types/array', context)
      assert_equal 'true - 1', value.strip
    end

    it 'does not replace with hash method' do
      context = { value: { name: 'hubot' } }
      value = subject.render('types/hash', context)
      assert_equal 'hubot -', value.strip
    end

    it 'replaces with struct method' do
      type = Struct.new(:name)
      context = { value: type.new('hubot') }
      value = subject.render('types/struct', context)
      assert_equal 'hubot - 1', value.strip
    end

    it 'replaces with true method' do
      context = { value: true }
      value = subject.render('types/boolean', context)
      assert_match /\d+/, value
    end

    it 'does not replace with false method' do
      context = { value: false}
      value = subject.render('types/boolean', context)
      assert_equal 'false', value.strip
    end
  end

  describe 'section tags' do
    it 'integer key value pushes onto context stack' do
      context = { value: -42 }
      value = subject.render('sections/dot', context)
      assert_equal '-42 42', value.strip
    end

    it 'true key value does not push context stack' do
      context = { value: true }
      value = subject.render('sections/true', context)
      assert_equal context.to_s, value.strip
    end
  end

  describe 'template error handling' do
    it 'raises for template not found' do
      assert_raises(ArgumentError) do
        subject.render('bogus', {})
      end
    end

    it 'raises for nil template name' do
      assert_raises(TypeError) do
        subject.render(nil, {})
      end
    end
  end

  describe 'escaping special characters' do
    it 'escapes characters in template text' do
      value = subject.render('escape', {})
      assert_equal "<kbd>\" \\n \"</kbd>\n", value
    end
  end
end
