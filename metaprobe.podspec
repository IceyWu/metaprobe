Pod::Spec.new do |s|
  s.name           = 'metaprobe'
  s.version        = '0.2.1'
  s.summary        = 'Cross-platform media metadata extraction powered by Rust'
  s.description    = 'Expo module bindings for local image and video metadata extraction.'
  s.license        = { :type => 'MIT', :file => 'LICENSE' }
  s.author         = { 'Lev Wu' => 'hi@levwu.me' }
  s.homepage       = 'https://github.com/IceyWu/metaprobe'
  s.platforms      = { :ios => '13.0' }
  s.source         = { :git => 'https://github.com/IceyWu/metaprobe.git', :tag => "metaprobe@#{s.version}" }
  s.module_name    = 'Metaprobe'
  s.swift_version  = '5.9'
  s.source_files   = 'ios/**/*.swift', 'Sources/MetaprobeSwift/**/*.swift'
  s.vendored_frameworks = 'Metaprobe.xcframework'
  s.dependency 'ExpoModulesCore'
end
