package logger

import (
	"fmt"
	"io"
	"os"
	"sync"
)

type LogLevel int

const (
	LogLevelDebug LogLevel = iota
	LogLevelInfo
	LogLevelWarn
	LogLevelError
	LogLevelFatal
)

var (
	logLevelStrings = [...]string{
		"DEBUG",
		"INFO",
		"WARN",
		"ERROR",
		"FATAL",
	}
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

	GetOutput() io.Writer
	SetOutput(io.Writer)
}

type logger struct {
	level      LogLevel
	output     io.Writer
	outputLock sync.RWMutex
	logLock    sync.Mutex
}

func New(level LogLevel) Logger {
	return &logger{level: level, output: os.Stdout}
}

func (l *logger) GetOutput() io.Writer {
	l.outputLock.RLock()
	defer l.outputLock.RUnlock()

	return l.output
}

func (l *logger) SetOutput(w io.Writer) {
	l.outputLock.Lock()
	defer l.outputLock.Unlock()

	l.output = w
}

func (l *logger) log(level LogLevel, args ...interface{}) {
	l.outputLock.RLock()
	defer l.outputLock.RUnlock()

	l.logLock.Lock()
	defer l.logLock.Unlock()

	if l.level <= level {
		fmt.Fprintf(l.output, "[%s] ", logLevelStrings[level])
		fmt.Fprintln(l.output, args...)
	}
}

func (l *logger) logf(level LogLevel, format string, args ...interface{}) {
	l.outputLock.RLock()
	defer l.outputLock.RUnlock()

	l.logLock.Lock()
	defer l.logLock.Unlock()

	if l.level <= level {
		fmt.Fprintf(l.output, "[%s] ", logLevelStrings[level])
		fmt.Fprintf(l.output, format, args...)
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
