// Uses the same matcher and ignorefile reader as Moby/BuildKit, not gitignore.
// https://github.com/moby/patternmatcher/tree/v0.6.1
package main

import (
 "encoding/json"
 "fmt"
 "io/fs"
 "os"
 "path/filepath"
 "sort"
 "github.com/moby/patternmatcher"
 "github.com/moby/patternmatcher/ignorefile"
)
func main() {
 root, err := filepath.Abs(os.Args[1]); must(err)
 f, err := os.Open(filepath.Join(root,".dockerignore")); must(err)
 patterns, err := ignorefile.ReadAll(f); must(err); must(f.Close())
 matcher, err := patternmatcher.New(patterns); must(err)
 excluded := []string{"packages/deck-builder/.env", "packages/deck-builder/data/deck-builder.db", "packages/deck-builder/data/agent-token", "packages/deck-builder/.dev/phase3/owner-online.db", "packages/deck-builder/test/hardening.test.ts", "packages/oracle-parser/package.json"}
 for _, name := range excluded { match,err:=matcher.MatchesOrParentMatches(name);must(err); if !match { panic("sensitive/unwanted path included: "+name) } }
 files:=[]string{}
 must(filepath.WalkDir(root,func(path string, d fs.DirEntry, err error) error {
  if err!=nil{return err}; rel,err:=filepath.Rel(root,path);if err!=nil{return err}
  if d.IsDir() {if d.Name()==".git" || d.Name()=="node_modules" {return filepath.SkipDir};return nil}
  match,err:=matcher.MatchesOrParentMatches(filepath.ToSlash(rel));if err!=nil{return err};if !match { files=append(files,filepath.ToSlash(rel)) };return nil
 }))
 sort.Strings(files)
 out,err:=json.MarshalIndent(map[string]any{"matcher":"github.com/moby/patternmatcher v0.6.1", "excluded_checks":excluded,"included_count":len(files),"included_files":files},"","  ");must(err);fmt.Println(string(out))
}
func must(err error){if err!=nil{panic(err)}}
