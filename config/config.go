package config

import (
	"io/ioutil"
	"regexp"

	"github.com/HoloArchivists/hoshinova/logger"
	"gopkg.in/yaml.v2"
)

type Regexp regexp.Regexp

func (r *Regexp) UnmarshalYAML(unmarshal func(interface{}) error) error {
	var s string
	if err := unmarshal(&s); err != nil {
		return err
	}
	re, err := regexp.Compile(s)
	if err != nil {
		return err
	}
	*r = Regexp(*re)
	return nil
}

type RawMessage struct {
	unmarshal func(interface{}) error
}

func (msg *RawMessage) UnmarshalYAML(unmarshal func(interface{}) error) error {
	msg.unmarshal = unmarshal
	return nil
}
func (msg *RawMessage) Unmarshal(v any) error {
	return msg.unmarshal(v)
}

type Config struct {
	App       AppConfig  `yaml:"app"`
	Channels  []Channel  `yaml:"channels"`
	Scrapers  []Scraper  `yaml:"scrapers"`
	Recorders []Recorder `yaml:"recorders"`
	Uploaders []Uploader `yaml:"uploaders"`
	Notifiers []Notifier `yaml:"notifiers"`
}

type AppConfig struct {
	LogLevel logger.LogLevel `yaml:"log_level"`
}

type Channel struct {
	Name    string   `yaml:"name"`
	Id      string   `yaml:"id"`
	Tags    Tags     `yaml:"tags"`
	Filters []Filter `yaml:"filters"`
}

type Filter struct {
	Regex Regexp `yaml:"regex"`
	Tags  Tags   `yaml:"tags"`
}

type Tags struct {
	Pub []string `yaml:"pub"`
	Sub []string `yaml:"sub"`
}

type ModuleConfig struct {
	// Name is an arbitrary human-readable identifier for the module.
	Name string `yaml:"name"`
	// Type defines what module to instantiate.
	Type string `yaml:"type"`
	// Tags list the task tags that this module should respond to.
	Tags Tags `yaml:"tags"`
	// Config is the module-specific configuration.
	Config RawMessage `yaml:"config"`
}

type Scraper struct {
	ModuleConfig `yaml:",inline"`
}

type Recorder struct {
	ModuleConfig `yaml:",inline"`
}

type Uploader struct {
	ModuleConfig `yaml:",inline"`
}

type Notifier struct {
	ModuleConfig `yaml:",inline"`
	Events       []string `yaml:"events"`
}

type YTArchive struct {
	ExecPath string   `yaml:"executable_path"`
	WorkDir  string   `yaml:"working_directory"`
	Flags    []string `yaml:"flags"`
	Quality  string   `yaml:"quality"`
}

func LoadConfig(path string) (*Config, error) {
	var config Config
	data, err := ioutil.ReadFile(path)
	if err != nil {
		return nil, err
	}
	err = yaml.UnmarshalStrict(data, &config)
	if err != nil {
		return nil, err
	}
	return &config, nil
}
