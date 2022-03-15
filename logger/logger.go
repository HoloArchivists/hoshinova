package logger

import "fmt"

type LogLevel int

const (
	LogLevelDebug LogLevel = iota
	LogLevelInfo
	LogLevelWarn
	LogLevelError
	LogLevelFatal
)

type Logger interface {
	Debug(args ...interface{})
	Info(args ...interface{})
	Warn(args ...interface{})
	Error(args ...interface{})
	Fatal(args ...interface{})

	Debugf(format string, args ...interface{})
	Infof(format string, args ...interface{})
	Warnf(format string, args ...interface{})
	Errorf(format string, args ...interface{})
	Fatalf(format string, args ...interface{})

	GetLogLevel() LogLevel
	SetLogLevel(level LogLevel)
}

type logger struct {
	level LogLevel
}

func New(level LogLevel) Logger {
	return &logger{level: level}
}

func (l *logger) log(level LogLevel, args ...interface{}) {
	if l.level <= level {
		fmt.Println(args...)
	}
}

func (l *logger) logf(level LogLevel, format string, args ...interface{}) {
	if l.level <= level {
		fmt.Printf(format, args...)
	}
}

func (l *logger) Debug(args ...interface{}) {
	l.log(LogLevelDebug, args...)
}

func (l *logger) Info(args ...interface{}) {
	l.log(LogLevelInfo, args...)
}

func (l *logger) Warn(args ...interface{}) {
	l.log(LogLevelWarn, args...)
}

func (l *logger) Error(args ...interface{}) {
	l.log(LogLevelError, args...)
}

func (l *logger) Fatal(args ...interface{}) {
	l.log(LogLevelFatal, args...)
}

func (l *logger) Debugf(format string, args ...interface{}) {
	l.logf(LogLevelDebug, format, args...)
}

func (l *logger) Infof(format string, args ...interface{}) {
	l.logf(LogLevelInfo, format, args...)
}

func (l *logger) Warnf(format string, args ...interface{}) {
	l.logf(LogLevelWarn, format, args...)
}

func (l *logger) Errorf(format string, args ...interface{}) {
	l.logf(LogLevelError, format, args...)
}

func (l *logger) Fatalf(format string, args ...interface{}) {
	l.logf(LogLevelFatal, format, args...)
}

func (l *logger) GetLogLevel() LogLevel {
	return l.level
}

func (l *logger) SetLogLevel(level LogLevel) {
	l.level = level
}
