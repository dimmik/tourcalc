import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

import InputLabel from '@material-ui/core/InputLabel';
import Input from '@material-ui/core/Input';
import MenuItem from '@material-ui/core/MenuItem';
import FormHelperText from '@material-ui/core/FormHelperText';
import FormControl from '@material-ui/core/FormControl';
import Select from '@material-ui/core/Select';
import TextField from '@material-ui/core/TextField';
import Chip from '@material-ui/core/Chip';
import Checkbox from '@material-ui/core/Checkbox';
import ListItemText from '@material-ui/core/ListItemText';
import FormControlLabel from '@material-ui/core/FormControlLabel';
import FormGroup from '@material-ui/core/FormGroup';
import Button from '@material-ui/core/Button';


export default class PersonForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open,
            person: props.person == null ? { name: "", weight: 100, parentId: "" } : JSON.parse(JSON.stringify(props.person)),
            tour: props.tour
        }
    }
    componentWillReceiveProps(props) {
        this.setState({
            dialogOpen: props.open,
            person: props.person == null ? { name: "", weight: 100, parentId: "" } : JSON.parse(JSON.stringify(props.person)),
            tour: props.tour
        });
    }

    render() {
        return (
            <span>
                <span onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.children}
                </span>
                <Dialog fullScreen={false} aria-labelledby="customized-dialog-title" open={this.state.dialogOpen} onClose={() => { this.setState({ dialogOpen: false }) }}>
                    <DialogTitle id="customized-dialog-title">{this.props.mode == 'edit' ? 'Edit' : 'Add'} Person</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                        }}>
                            <FormGroup>
                                <TextField
                                    id="name"
                                    required
                                    label="Name"
                                    defaultValue={this.state.person.name}
                                    autoFocus
                                    onChange={(e) => { this.state.person.name = e.target.value; this.setState({ person: this.state.person }) }}
                                    margin="normal"
                                />
                                <TextField
                                    id="weight"
                                    required
                                    label="Weight"
                                    type="number"
                                    defaultValue={this.state.person.weight}
                                    onChange={(e) => { this.state.person.weight = e.target.value; this.setState({ person: this.state.person }) }}
                                    margin="normal"
                                />
                                <br />
                                <InputLabel htmlFor="select-relative">Paying Relative</InputLabel>
                                <Select
                                    input={<Input id="select-relative" />}
                                    value={this.state.person.parentId == null ? 'none' : this.state.person.parentId}
                                    onChange={(e) => { this.state.person.parentId = e.target.value; this.setState({ person: this.state.person }) }}
                                >
                                    <MenuItem value="none" key='None'>None</MenuItem>
                                    {
                                        this.state.tour.persons.map(p => (<MenuItem value={p.guid} key={p.guid}>{p.name}</MenuItem>))
                                    }
                                </Select>
                                <br />
                            </FormGroup>

                        </form>
                </DialogContent>
                <DialogActions>
                        <Button
                            color="primary" size='large' variant='outlined' 
                            onClick={() => {
                            (  this.props.mode === "add"
                                ? AppState.addPerson(this.props.app, this.props.tourid, this.state.person)
                                : AppState.editPerson(this.props.app, this.props.tourid, this.state.person)
                            )
                                .then(this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTour(this.props.app, this.props.tourid) })

                        }}>{this.props.actionButtonText}</Button>
                        <Button
                            color="secondary" size='large' variant='outlined' 
                            onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</Button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}
