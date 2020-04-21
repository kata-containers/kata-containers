package log

import (
	"fmt"
	"log"
)

// Logger is the interface that the loggers used by the library will use.
type Logger interface {
	Infof(format string, args ...interface{})
	Warningf(format string, args ...interface{})
	Errorf(format string, args ...interface{})
	Debugf(format string, args ...interface{})
}

// Dummy logger doesn't log anything
var Dummy = &dummy{}

type dummy struct{}

func (d *dummy) Infof(format string, args ...interface{})    {}
func (d *dummy) Warningf(format string, args ...interface{}) {}
func (d *dummy) Errorf(format string, args ...interface{})   {}
func (d *dummy) Debugf(format string, args ...interface{})   {}

// Std is a wrapper for go standard library logger.
type Std struct {
	Debug bool
}

func (s *Std) logWithPrefix(prefix, format string, args ...interface{}) {
	format = fmt.Sprintf("%s %s", prefix, format)
	log.Printf(format, args...)
}

func (s *Std) Infof(format string, args ...interface{}) {
	s.logWithPrefix("[INFO]", format, args...)
}
func (s *Std) Warningf(format string, args ...interface{}) {
	s.logWithPrefix("[WARN]", format, args...)
}
func (s *Std) Errorf(format string, args ...interface{}) {
	s.logWithPrefix("[ERROR]", format, args...)
}
func (s *Std) Debugf(format string, args ...interface{}) {
	if s.Debug {
		s.logWithPrefix("[DEBUG]", format, args...)
	}
}
