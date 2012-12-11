/*
Copyright 2011-2012 Paul Ruane.

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

package commands

import (
	"fmt"
	"tmsu/cli"
	"tmsu/storage"
)

type StatsCommand struct{}

func (StatsCommand) Name() string {
	return "stats"
}

func (StatsCommand) Synopsis() string {
	return "Show database statistics"
}

func (StatsCommand) Description() string {
	return `tmsu stats

Shows the database statistics.`
}

func (StatsCommand) Options() []cli.Option {
	return []cli.Option{}
}

func (StatsCommand) Exec(args []string) error {
	store, err := storage.Open()
	if err != nil {
		return err
	}
	defer store.Close()

	tagCount, err := store.Db.TagCount()
	if err != nil {
		return err
	}

	fileCount, err := store.FileCount()
	if err != nil {
		return err
	}

	fileTagCount, err := store.Db.FileTagCount()
	if err != nil {
		return err
	}

	fmt.Printf("Database Contents\n")

	fmt.Printf(" Tags:      %v\n", tagCount)
	fmt.Printf(" Files:     %v\n", fileCount)
	fmt.Printf(" Taggings:  %v\n", fileTagCount)
	fmt.Printf(" Average:   %v\n", fileTagCount/fileCount)

	return nil
}
