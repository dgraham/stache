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
      Context = Struct.new(:name)
      context = Context.new({ 'login' => 'hubot' })
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
