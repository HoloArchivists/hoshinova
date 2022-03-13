package config

import (
	"io/ioutil"
	"regexp"

	"gopkg.in/yaml.v2"
)

type Regexp regexp.Regexp

type Config struct {
	Channels     []Channel `yaml:"channels"`
	PollInterval int       `yaml:"poll_interval"`
}

type Channel struct {
	Name      string   `yaml:"name"`
	ChannelID string   `yaml:"id"`
	Filters   []Regexp `yaml:"filters"`
}

func LoadConfig() (*Config, error) {
	path := "config.yaml"
	var config Config
	data, err := ioutil.ReadFile(path)
	if err != nil {
		return nil, err
	}
	err = yaml.Unmarshal(data, &config)
	if err != nil {
		return nil, err
	}
	return &config, nil
}

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
