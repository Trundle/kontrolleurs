function _kontrolleurs_ctrl_r
  if history -z | kontrolleurs | read -zl execute cursor match
    commandline -rb $match
    commandline -f repaint
    commandline -C $cursor
    if test $execute = true
      commandline -f execute
    end
  else
    commandline -f repaint
  end
end
