// Clean: Proper error handling
package main

import "errors"

func process(value int) error {
	if value < 0 {
		return errors.New("negative value not allowed")
	}
	return nil
}
